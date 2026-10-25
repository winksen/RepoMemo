# Certificates for the edge

Put your own certificate here to make the `edge` profile use it instead of getting one:

- `tls.crt`: the certificate, with its chain;
- `tls.key`: its private key.

Both or neither. The folder is mounted read-only at `/certs` in the edge container (`REPOMEMO_TLS_DIR` in `.env` points
at another folder). Nothing here but this file is committed: `.gitignore` keeps keys out of the repository.
