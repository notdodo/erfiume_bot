.PHONY: format format-check lint install test type-check check help

format: ## Format repository code
	poetry run ruff format
	poetry run ruff check --fix

format-check: ## Check the code format with no actual side effects
	poetry run ruff format --check

lint: ## Launch the linting tools
	poetry run ruff check

install: ## Install Python dependencies
	poetry install --no-root

test:
	@echo "Not Implemented"

type-check: ## Launch the type checking tool
	poetry run mypy .

check: format-check lint type-check ## Launch all the checks (formatting, linting, type checking)

help: ## Show the available commands
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'