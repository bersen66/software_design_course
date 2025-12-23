# Простой gRPC-чат

Простая реализация мессенджера с клиент-серверной архитектурой, TUI и
обменом сообщениями через gRPC.

# Инструкция по запуску
## Сервер
```bash
$ export PORT=<port>
$ export CHAT_DB=<path_to_sqlite-db>
$ go run cmd/server/main.go
```

## Клиент
```bash
$ go run cmd/client/main.go -addr <addr:port> -user <username> -room <chatroom_name>
```

## Запуск тестов
```bash
$ go test -v ./...
```
