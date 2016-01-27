#include <iostream>

#include <boost/asio/connect.hpp>
#include <boost/asio/io_service.hpp>
#include <boost/asio/ip/tcp.hpp>
#include <boost/asio/write.hpp>
#include <boost/lexical_cast.hpp>

#include <msgpack.hpp>

#define MSGPACK

int main(int argc, char** argv) {
    if (argc < 3) {
        std::cerr << "Use: PROGRAM HOST PORT [COUNT=1]" << std::endl;
        return 1;
    }

    const uint count = argc == 4 ? boost::lexical_cast<uint>(argv[3]) : 1;

    boost::asio::io_service loop;
    boost::asio::ip::tcp::socket socket(loop);
    boost::asio::ip::tcp::resolver resolver(loop);
    boost::asio::connect(socket, resolver.resolve({ argv[1], argv[2] }));

#ifdef JSON
    std::string message = "{\"id\":42,\"source\":\"service\",\"parent\":{\"child\":\"item\"},\"message\":\"le message - ";
    std::string data;
    data.reserve(512);

    for (uint i = 0; i < count; ++i) {
        data.assign(message);
        data.append(boost::lexical_cast<std::string>(i));
        data.append("\"}");
        boost::asio::write(socket, boost::asio::buffer(data));
    }
#endif

#ifdef MSGPACK
    msgpack::sbuffer sbuf;

    for (uint i = 0; i < count; ++i) {
        msgpack::packer<msgpack::sbuffer> packer(&sbuf);
        packer.pack_map(4);
        packer.pack(std::string("id"));
        packer.pack(42);
        packer.pack(std::string("source"));
        packer.pack(std::string("app/echo"));
        packer.pack(std::string("parent"));
        packer.pack_map(1);
        packer.pack(std::string("child"));
        packer.pack(std::string("item"));
        packer.pack(std::string("message"));
        packer.pack(std::string("le message - ") + boost::lexical_cast<std::string>(i));

        boost::asio::write(socket, boost::asio::buffer(sbuf.data(), sbuf.size()));
        sbuf.clear();
    }
#endif

    return 0;
}
