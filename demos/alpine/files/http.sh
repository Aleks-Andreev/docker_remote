#!/bin/sh

cd /root/app

CONTENT_LENGTH=`cat index.html | wc -c`
CONTENT=`cat index.html`
PORT=8084

trap "exit 0" TERM INT
echo "Listening on $PORT"

while true;
do
    printf "HTTP/1.1 200 OK\r\nContent-Length: $CONTENT_LENGTH\r\n\r\n$CONTENT\n" | nc -l -p $PORT &
    wait $!
done
