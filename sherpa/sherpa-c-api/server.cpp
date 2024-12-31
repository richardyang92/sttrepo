#include "server.h"
#include <arpa/inet.h>
#include <cstring>
#include <iostream>
#include <event2/util.h>

Server::Server(int port, AcceptCallback acceptCb, ErrorCallback errorCb, EventHandler *eventHandler)
    : port_(port), acceptCb_(std::move(acceptCb)), errorCb_(std::move(errorCb)), eventHandler_(std::move(eventHandler)) {
    base_ = event_base_new();
    if (!base_) {
        std::cerr << "Failed to create event base" << std::endl;
        exit(1);
    }

    struct sockaddr_in sin;
    memset(&sin, 0, sizeof(sin));
    sin.sin_family = AF_INET;
    sin.sin_addr.s_addr = htonl(INADDR_ANY);
    sin.sin_port = htons(port_);

    listener_ = evconnlistener_new_bind(base_,
        [](struct evconnlistener *listener, evutil_socket_t fd, struct sockaddr *address, int socklen, void *ctx) {
            auto server = static_cast<Server*>(ctx);
            server->handleAccept(listener, fd, address, socklen);
        },
        this, LEV_OPT_CLOSE_ON_FREE | LEV_OPT_REUSEABLE, -1, (struct sockaddr*)&sin, sizeof(sin));
    if (!listener_) {
        std::cerr << "Failed to create listener" << std::endl;
        event_base_free(base_);
        exit(1);
    }

    evconnlistener_set_error_cb(listener_, [](struct evconnlistener *listener, void *ctx) {
        auto server = static_cast<Server*>(ctx);
        server->handleError(listener);
    });
}

Server::~Server() {
    event_base_free(base_);
}

void Server::start() {
    event_base_dispatch(base_);
}

void Server::handleAccept(struct evconnlistener *listener, evutil_socket_t fd, struct sockaddr *address, int socklen) {
    struct event_base *base = evconnlistener_get_base(listener);
    struct bufferevent *bev = bufferevent_socket_new(base, fd, BEV_OPT_CLOSE_ON_FREE);

    bufferevent_setcb(bev, this->eventHandler_->read_cb, nullptr, this->eventHandler_->event_cb, nullptr);

    bufferevent_enable(bev, EV_READ | EV_WRITE);

    if (acceptCb_) {
        acceptCb_(listener, fd, address, socklen, this);
    }
}

void Server::handleError(struct evconnlistener *listener) {
    int err = EVUTIL_SOCKET_ERROR();
    std::cerr << "Got an error " << err << " (" << evutil_socket_error_to_string(err) << ") on the listener. Shutting down." << std::endl;

    if (errorCb_) {
        errorCb_(listener, this);
    }

    event_base_loopexit(base_, nullptr);
}

ServerBuilder& ServerBuilder::setPort(int port) {
    port_ = port;
    return *this;
}

ServerBuilder& ServerBuilder::setAcceptCallback(const std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)>& acceptCb) {
    acceptCb_ = acceptCb;
    return *this;
}

ServerBuilder& ServerBuilder::setErrorCallback(const std::function<void(struct evconnlistener*, void*)>& errorCb) {
    errorCb_ = errorCb;
    return *this;
}

ServerBuilder& ServerBuilder::setEventHandler(std::unique_ptr<EventHandler> eventHandler) {
    eventHandler_ = std::move(eventHandler);
    return *this;
}

std::unique_ptr<Server> ServerBuilder::build() {
    return std::make_unique<Server>(port_, acceptCb_, errorCb_, eventHandler_.get());
}