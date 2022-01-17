FROM rust:1.58 as builder

WORKDIR /requester

COPY . .

RUN cargo build --release
RUN ls -a

FROM debian:buster-slim
ARG APP=/var/requester

RUN apt-get update \
    && apt-get install -y libssl-dev pkg-config build-essential

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=builder /requester/target/release/povorot-requester ${APP}

RUN chown -R $APP_USER:$APP_USER ${APP}
USER $APP_USER
WORKDIR ${APP}
CMD [ "./povorot-requester" ]