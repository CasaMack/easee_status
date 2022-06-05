FROM alpine as stripper

RUN apk add binutils
RUN apk --no-cache add ca-certificates

COPY easee_status /easee_status
RUN strip /easee_status

FROM scratch as run

COPY --from=stripper /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=stripper /easee_status /easee_status

CMD ["/easee_status"]
