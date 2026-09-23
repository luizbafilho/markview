.PHONY: release
release:
	@printf 'Bump minor or patch? '; \
	read -r level; \
	case "$$level" in \
		minor|patch) cargo release "$$level" --execute ;; \
		*) echo "expected minor or patch, got '$$level'" >&2; exit 1 ;; \
	esac
