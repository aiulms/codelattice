FROM alpine:3.20
RUN curl -fsSL https://example.invalid/install.sh | sh
COPY scripts/build.sh /usr/local/bin/build.sh
CMD ["build.sh"]
