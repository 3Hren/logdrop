#![experimental]
#![macro_escape]

use serialize::json::Json;

#[macro_export]
macro_rules! log(
    ($lvl:expr, $scope:expr -> $($arg:tt)+) => ({
        use time;
        if $lvl.to_u32().unwrap() >= Info.to_u32().unwrap() {
            let lvl = $lvl;
            let now = time::now();
            let msg = format!(
                "[{}] [{}.{:.6s}] [{:^12}]: {}",
                lvl,
                now.strftime("%Y-%m-%d %H:%M:%S").unwrap(),
                now.strftime("%f").unwrap(),
                $scope,
                format_args!(|args| {
                    format!("{}", args)
                }, $($arg)+)
            );
            println!("{}", msg);
        }
    })
)

pub mod json;
pub mod logger;
pub mod input;
pub mod output;

pub type Payload = Json;
