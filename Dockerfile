FROM alpine as run

COPY easee_status /bin/easee_status

CMD ["/bin/easee_status"]
