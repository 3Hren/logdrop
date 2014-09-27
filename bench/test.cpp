#include <boost/asio.hpp>
#include <boost/lexical_cast.hpp>

int main(int argc, char** argv) {
    boost::asio::io_service loop;
    boost::asio::ip::tcp::socket socket(loop);
    boost::asio::ip::tcp::resolver resolver(loop);
    boost::asio::connect(socket, resolver.resolve({ argv[1], argv[2] }));

    std::string message = "{\"id\":42,\"source\":\"service\",\"parent\":{\"child\":\"item\"},\"message\":\"le message - ";
    std::string data;
    data.reserve(512);
    uint count = boost::lexical_cast<uint>(argv[3]);
    for (uint i = 0; i < count; ++i) {
        data.assign(message);
        data.append(boost::lexical_cast<std::string>(i));
        data.append("\"}");
        boost::asio::write(socket, boost::asio::buffer(data));
    }
    return 0;
}
