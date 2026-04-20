.PHONY: up seed test down logs clean

up:
	docker compose up -d
	@echo "Waiting for crapi-web to become healthy..."
	@for i in $$(seq 1 120); do \
		if docker compose ps crapi-web | grep -q healthy; then \
			echo "Services healthy after $${i}s"; \
			break; \
		fi; \
		if [ "$$i" -eq 120 ]; then \
			echo "ERROR: crapi-web did not become healthy in 120s"; \
			exit 1; \
		fi; \
		sleep 1; \
	done

seed:
	@echo "No seed step needed — crAPI auto-seeds on first boot"

test:
	docker compose --profile test up -d playwright
	docker compose exec -T playwright sh -c 'cd /work/tests/integration/level2 && npm ci --prefer-offline --no-audit && npx playwright test'

down:
	docker compose down -v

logs:
	docker compose logs -f

clean: down
	rm -rf out/
