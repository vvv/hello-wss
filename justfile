# -*- makefile -*-

# Generate self-signed certificate and private key
gencert:
    #!/usr/bin/env bash
    set -eu -o pipefail

    CRT=test.cer
    KEY=test.key

    cd {{justfile_directory()}}

    rm -f $CRT $KEY

    # `-nodes` ("no DES") --- don't protect the private key with passphrase
    openssl req -new -x509 -newkey rsa:4096 -sha256 \
        -days 365 \
        -out $CRT \
        -keyout $KEY -nodes \
        -subj '/CN=example.com'

    # NOTE: If SAN (Subject Alternate Name) is needed, use the command from
    # https://stackoverflow.com/a/41366949

    #XXX-DELETEME openssl pkcs12 -export -nodes -in $CRT -inkey $KEY -out test.p12 \
    #XXX-DELETEME     -passout pass:  # no password
