CREATE TABLE charts (
    token_id        VARCHAR(42) NOT NULL,
    interval        VARCHAR(5) NOT NULL,
    time            BIGINT NOT NULL,
    open            NUMERIC NOT NULL,
    high            NUMERIC NOT NULL,
    low             NUMERIC NOT NULL,
    close           NUMERIC NOT NULL,
    volume          NUMERIC NOT NULL,
    PRIMARY KEY (token_id, interval, time)
);
