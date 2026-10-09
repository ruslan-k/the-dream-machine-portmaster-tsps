FROM debian:bookworm-slim
RUN dpkg --add-architecture i386 && apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      wine wine32 xvfb python3 innoextract procps && \
    rm -rf /var/lib/apt/lists/*
COPY dump_swfs.py recover_zinc.py /tools/
ENTRYPOINT ["python3", "/tools/recover_zinc.py"]
