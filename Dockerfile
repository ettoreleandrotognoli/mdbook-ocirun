FROM fedora
RUN dnf install -y \
    diffutils \
    && dnf clean all \
    && rm -rf /var/cache/yum