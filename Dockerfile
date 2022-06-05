FROM alpine as stripper

RUN apk add binutils

COPY easee_status /easee_status
RUN strip /easee_status

FROM scratch as run

COPY --from=stripper /easee_status /easee_status

CMD ["/easee_status"]
