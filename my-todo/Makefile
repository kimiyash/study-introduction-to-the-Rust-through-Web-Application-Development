build:
	docker compose build
up:
	docker compose up -d

down:
	docker compose down

login:
	docker compose exec database bash

db-login:
	docker compose exec database psql -U admin todos

dev:
	sqlx db create
	sqlx migrate run
	cargo watch -x run

test:
	cargo test

fmt:
	cargo fmt

