FROM alpine
VOLUME /app/data

RUN apk add --no-cache ffmpeg
RUN apk add --no-cache opus-tools
COPY annil /app/
COPY anni /app/

WORKDIR /app/data
ENTRYPOINT ["/app/annil"]