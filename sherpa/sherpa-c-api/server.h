#ifndef SERVER_H
#define SERVER_H

#include <event2/listener.h>
#include <event2/bufferevent.h>
#include <event2/buffer.h>
#include <event2/event.h>
#include <functional>
#include <memory>

class Server;
struct EventHandler;

class ServerBuilder {
public:
    ServerBuilder& setPort(int port);
    ServerBuilder& setAcceptCallback(const std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)>& acceptCb);
    ServerBuilder& setErrorCallback(const std::function<void(struct evconnlistener*, void*)>& errorCb);
    ServerBuilder& setEventHandler(std::unique_ptr<EventHandler> eventHandler);
    std::unique_ptr<Server> build();

private:
    friend class Server; // 允许Server访问ServerBuilder的私有成员

    int port_;
    std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)> acceptCb_;
    std::function<void(struct evconnlistener*, void*)> errorCb_;
    std::unique_ptr<EventHandler> eventHandler_;
};

class Server {
public:
    using AcceptCallback = std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)>;
    using ErrorCallback = std::function<void(struct evconnlistener*, void*)>;

    Server(int port, AcceptCallback acceptCb = nullptr, ErrorCallback errorCb = nullptr, EventHandler *eventHandler = nullptr);
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
    struct EventHandler *eventHandler_;
};

struct EventHandler {
    void (*read_cb) (struct bufferevent *bev, void *);
    void (*event_cb) (struct bufferevent *, short, void *);
};

#endif // SERVER_H