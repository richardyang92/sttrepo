#ifndef SERVER_H
#define SERVER_H

#include <event2/listener.h>
#include <event2/bufferevent.h>
#include <event2/buffer.h>
#include <event2/event.h>
#include <functional>
extern "C" {
    #include <sherpa.h>
}

namespace server {
    namespace sherpa {
        class SherpaHandleWrapper {
        private:
            // 定义一个智能指针类型，指向sherpa_handle
            std::shared_ptr<SherpaHandle> handle;
            // 定义一个线程安全的bool类型，用于标记handle是否在使用中
            std::shared_ptr<std::atomic<bool>> inUse;
        public:
            SherpaHandleWrapper(std::shared_ptr<SherpaHandle> handle, std::shared_ptr<std::atomic<bool>> inUse);
            ~SherpaHandleWrapper();
            // 定义一个函数，用于获取Sherpa句柄
            SherpaHandle getHandle() const;
            // 定义一个函数，用于设置inUse的值
            void setInUse(bool value) const;
            // 定义一个函数，用于判断handle是否在使用中
            bool isInUse() const;
        };
        
        class SherpaPool {
        private:
            int total_size;
            std::vector<SherpaHandleWrapper> handles;
        public:
            SherpaPool(int total_size);
            ~SherpaPool();
            SherpaHandleWrapper *selectHandle();
            void releaseHandle(SherpaHandleWrapper *handle);
        };
    };

    class Server;

    class ServerBuilder {
    public:
        ServerBuilder& setPort(int port);
        ServerBuilder& setAcceptCallback(const std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)>& acceptCb);
        ServerBuilder& setErrorCallback(const std::function<void(struct evconnlistener*, void*)>& errorCb);
        std::unique_ptr<Server> build();

    private:
        friend class Server; // 允许Server访问ServerBuilder的私有成员

        int port_;
        std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)> acceptCb_;
        std::function<void(struct evconnlistener*, void*)> errorCb_;
    };

    class Server {
    public:
        using AcceptCallback = std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)>;
        using ErrorCallback = std::function<void(struct evconnlistener*, void*)>;

        Server(int port, AcceptCallback acceptCb = nullptr, ErrorCallback errorCb = nullptr);
        ~Server();
        void start();

    private:
        void handleAccept(struct evconnlistener *listener, evutil_socket_t fd, struct sockaddr *address, int socklen);
        void handleError(struct evconnlistener *listener);

        int port_;
        struct event_base *base_;
        struct evconnlistener *listener_;
        AcceptCallback acceptCb_;
        ErrorCallback errorCb_;
    };

    struct Connection {
        int connectionId;
        sherpa::SherpaHandleWrapper *sherpaWrapper;
        std::unique_ptr<float[]> samples;
        Connection(int id, sherpa::SherpaHandleWrapper *wrapper) {
            connectionId = id;
            sherpaWrapper = wrapper;
            samples = std::make_unique<float[]>(4096);
        }
    };
};

#endif // SERVER_H