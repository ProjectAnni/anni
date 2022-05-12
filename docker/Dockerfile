FROM alpine
VOLUME /app/data

RUN apk add --no-cache ffmpeg
COPY annil /app/
COPY anni /app/

WORKDIR /app/data
ENTRYPOINT ["/app/annil"]