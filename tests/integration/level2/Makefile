.PHONY: up seed down logs clean

up:
	docker compose up -d
	@echo "Waiting for services..."
	@for i in $$(seq 1 90); do \
		if docker compose ps laravel-api | grep -q healthy; then \
			echo "Services healthy after $${i}s"; \
			break; \
		fi; \
		if [ "$$i" -eq 90 ]; then \
			echo "ERROR: Services did not become healthy in 90s"; \
			exit 1; \
		fi; \
		sleep 1; \
	done

seed:
	docker compose exec laravel-api php artisan migrate:refresh --seed
	@echo "Seed complete"

down:
	docker compose down -v

logs:
	docker compose logs -f

clean: down
	rm -rf out/
