FROM rust:latest

WORKDIR /usr/src/update-chat-types
COPY . .

RUN cargo install --path .

ENTRYPOINT ["update-chat-types"]
