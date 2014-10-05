#![allow(non_camel_case_types)] // C types

use std::collections::{HashSet};
use std::c_str::CString;
use std::io::{IoError, IoResult};
use std::io::{BufferedReader, File, Open, ReadWrite};
use std::mem::{transmute};
use std::ptr;
use std::raw::Slice;
use std::os;

use libc::{c_void, c_char, c_int, ENOENT};

use sync::{Arc, Mutex};

#[repr(C)]
enum CFStringBuiltInEncodings {
    kCFStringEncodingUnicode = 0x0100,
    kCFStringEncodingUTF8    = 0x08000100,
}

pub enum Event {
    Created,
    Removed,
    //ModifiedMeta,
    Modified,
    RenamedOld,
    RenamedNew,
}

enum Control {
    Update(HashSet<String>),
    Exit,
}

#[repr(C)]
struct FSEventStreamContext {
    version: c_int,
    info: *mut c_void,
    retain: *const c_void,
    release: *const c_void,
    desc: *const c_void,
}

type callback_t = extern "C" fn(
    stream: *const c_void,
    info: *const c_void,
    size: c_int,
    paths: *const *const i8,
    events: *const u32,
    ids: *const u64
);

#[repr(C)]
enum FSEventStreamEventFlags {
    kFSEventStreamEventFlagItemCreated  = 0x00000100,
    kFSEventStreamEventFlagItemRemoved  = 0x00000200,
    kFSEventStreamEventFlagItemRenamed  = 0x00000800,
    kFSEventStreamEventFlagItemModified = 0x00001000,
    kFSEventStreamEventFlagItemIsFile   = 0x00010000,
}

extern "C"
fn callback(stream: *const c_void,
            info: *const c_void,
            size: c_int,
            paths: *const *const i8,
            events: *const u32,
            ids: *const u64)
{
    let tx: &mut Sender<(Event, String)> = unsafe {
        &mut *(info as *mut Sender<(Event, String)>)
    };

    let events: &[u32] = unsafe {
        transmute(Slice {
            data: events,
            len: size as uint,
        })
    };

    let ids: &[u64] = unsafe {
        transmute(Slice {
            data: ids,
            len: size as uint,
        })
    };

    let paths: &[*const i8] = unsafe {
        transmute(Slice {
            data: paths,
            len: size as uint,
        })
    };

    let mut paths_ : Vec<CString> = Vec::new();
    for path in paths.iter() {
        paths_.push(unsafe { CString::new(*path, false) });
    }

    let mut renamed = false;
    for id in range(0, size as uint) {
        debug!("event: {}, id: {}, path: {}", events[id], ids[id], paths_[id]);
        let event = events[id];
        let path = String::from_str(paths_[id].as_str().unwrap());

        if event & kFSEventStreamEventFlagItemIsFile as u32 == 0 {
            continue;
        }

        if event & kFSEventStreamEventFlagItemCreated as u32 > 0 {
            tx.send((Created, path));
        } else if event & kFSEventStreamEventFlagItemRemoved as u32 > 0 {
            tx.send((Removed, path));
        } else if event & kFSEventStreamEventFlagItemRenamed as u32 > 0 {
            if renamed {
                tx.send((RenamedNew, path));
            } else {
                tx.send((RenamedOld, path));
            }
            renamed = !renamed;
        } else if event & kFSEventStreamEventFlagItemModified as u32 > 0 {
            tx.send((Modified, path));
        }
    }
}

struct CoreFoundationString {
    d: *const c_void,
}

impl CoreFoundationString {
    fn new(string: &str) -> CoreFoundationString {
        CoreFoundationString {
            d: unsafe {
                CFStringCreateWithCString(
                    kCFAllocatorDefault,
                    string.to_c_str().as_ptr(),
                    kCFStringEncodingUTF8
                )
            }
        }
    }
}

impl Drop for CoreFoundationString {
    fn drop(&mut self) {
        unsafe { CFRelease(self.d) }
    }
}

struct CoreFoundationArray {
    d: *const c_void,
    #[allow(dead_code)] items: Vec<CoreFoundationString>, // It's a RAII container.
}

impl CoreFoundationArray {
    fn new(collection: &HashSet<String>) -> CoreFoundationArray {
        let d = unsafe {
            CFArrayCreateMutable(
                kCFAllocatorDefault,
                collection.len() as i32,
                ptr::null::<c_void>()
            )
        };

        let mut items = Vec::new();
        for item in collection.iter() {
            let item = CoreFoundationString::new(item.as_slice());
            unsafe {
                CFArrayAppendValue(d, item.d);
            }
            items.push(item);
        }

        CoreFoundationArray {
            d: d,
            items: items,
        }
    }
}

impl Drop for CoreFoundationArray {
    fn drop(&mut self) {
        unsafe { CFRelease(self.d) }
    }
}

fn recreate_stream(eventloop: *mut c_void, context: *const FSEventStreamContext, paths: HashSet<String>) -> *mut c_void {
    let paths = CoreFoundationArray::new(&paths);

    let stream = unsafe {
        FSEventStreamCreate(
            kCFAllocatorDefault,
            callback,
            context,
            paths.d,
            0xFFFFFFFFFFFFFFFFu64,
            0.0f64,
            0x00000010u32
        )
    };

    unsafe {
        FSEventStreamRetain(stream);
        FSEventStreamScheduleWithRunLoop(stream, eventloop, kCFRunLoopDefaultMode);
        FSEventStreamStart(stream);
        FSEventStreamFlushAsync(stream);
        stream
    }
}

pub struct Watcher {
    pub rx: Receiver<(Event, String)>,
    ctx: SyncSender<Control>,
    paths: HashSet<String>,
    stream: Arc<Mutex<*mut c_void>>,
    eventloop: Arc<Mutex<*mut c_void>>,
}

impl Watcher {
    pub fn new() -> Watcher {
        let (mut tx, rx) = channel::<(Event, String)>();
        let (ctx, crx) = sync_channel::<Control>(0);

        let eventloop = Arc::new(Mutex::new(ptr::mut_null::<c_void>()));
        let stream = Arc::new(Mutex::new(ptr::mut_null::<c_void>()));

        let watcher = Watcher {
            rx: rx,
            ctx: ctx,
            paths: HashSet::new(),
            stream: stream.clone(),
            eventloop: eventloop.clone(),
        };

        spawn(proc() {
            unsafe {
                *eventloop.lock() = CFRunLoopGetCurrent();

                let tx: *mut c_void = &mut tx as *mut _ as *mut c_void;
                let context = FSEventStreamContext {
                    version: 0,
                    info: tx,
                    retain: ptr::null::<c_void>(),
                    release: ptr::null::<c_void>(),
                    desc: ptr::null::<c_void>(),
                };

                loop {
                    debug!("recycle");
                    match crx.recv() {
                        Update(paths) => {
                            *stream.lock() = recreate_stream(*eventloop.lock(), &context, paths);
                            CFRunLoopRun();
                        }
                        Exit => break
                    }
                }
            }
        });

        watcher
    }

    pub fn watch(&mut self, path: &Path) -> IoResult<()> {
        if path.exists() {
            debug!("adding {} to watch", path.display());
            let path = os::make_absolute(path);
            let path = match path.as_str() {
                Some(path) => String::from_str(path),
                None => return Err(IoError::from_errno(ENOENT as uint, false))
            };
            self.paths.insert(path.clone());
            self.update();
            Ok(())
        } else {
            Err(IoError::from_errno(ENOENT as uint, false))
        }
    }

    pub fn unwatch(&mut self, path: &String) -> IoResult<()> {
        self.paths.remove(path);
        self.update();
        Ok(())
    }

    fn update(&self) {
        self.stop_stream();
        self.ctx.send(Update(self.paths.clone()));
    }

    fn stop_stream(&self) {
        let mut stream = self.stream.lock();
        if !(*stream).is_null() {
            unsafe {
                FSEventStreamStop(*stream);
                FSEventStreamUnscheduleFromRunLoop(*stream, *self.eventloop.lock(), kCFRunLoopDefaultMode);
                FSEventStreamInvalidate(*stream);
                FSEventStreamRelease(*stream);
                CFRunLoopWakeUp(*self.eventloop.lock());
            }
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        debug!("dropping! {:p}", self);
        self.stop_stream();
        self.ctx.send(Exit);
    }
}

#[link(name = "Carbon", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
extern {
    static kCFAllocatorDefault: *mut c_void;
    static kCFRunLoopDefaultMode: *mut c_void;

    fn CFStringCreateWithCString(allocator: *mut c_void, string: *const c_char, encoding: CFStringBuiltInEncodings) -> *const c_void;

    fn CFArrayCreateMutable(allocator: *mut c_void, size: c_int, callbacks: *const c_void) -> *const c_void;
    fn CFArrayAppendValue(array: *const c_void, value: *const c_void);

    fn FSEventStreamCreate(allocator: *mut c_void, cb: callback_t, context: *const FSEventStreamContext, paths: *const c_void, since: u64, latency: f64, flags: u32) -> *mut c_void;

    fn FSEventStreamRetain(stream: *mut c_void);
    fn FSEventStreamScheduleWithRunLoop(stream: *mut c_void, eventloop: *mut c_void, mode: *mut c_void);
    fn FSEventStreamUnscheduleFromRunLoop(stream: *mut c_void, eventloop: *mut c_void, mode: *mut c_void);
    fn FSEventStreamStart(stream: *mut c_void);
    fn FSEventStreamStop(stream: *mut c_void);
    fn FSEventStreamInvalidate(stream: *mut c_void);
    fn FSEventStreamRelease(stream: *mut c_void);
    fn FSEventStreamFlushAsync(stream: *mut c_void);

    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopRun();
    fn CFRunLoopWakeUp(ev: *mut c_void);

    fn CFRelease(p: *const c_void);
}

#[test]
fn main() {
    let path = Path::new("/tmp/logstash.log");
    let mut watcher = Watcher::new();
    watcher.watch(&path).unwrap();
//    watcher.watch(Path::new("/Users/esafronov/sandbox")).unwrap();

    let file = match File::open_mode(&path, Open, ReadWrite) {
        Ok(f) => f,
        Err(e) => fail!("file error: {}", e),
    };
    let mut reader = BufferedReader::new(file);
    loop {
        for line in reader.lines() {
            debug!("{}", line.unwrap());
        }

        match watcher.rx.recv() {
            (Created, path)  => { debug!("received create event: {}", path); }
            (Removed, path)  => { debug!("received remove event: {}", path); }
            (Modified, path) => { debug!("received modify event: {}", path); }
            (RenamedOld, path) => { debug!("received renamed old event: {}", path); }
            (RenamedNew, path) => { debug!("received renamed new event: {}", path); }
        }
    }
}
