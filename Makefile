.PHONY: populate-sqlite

populate-sqlite:
	cd cedict_dictionary && go run -tags "fts5" ./cmd/scripts/populate-to-sqlite/main.go

open-dictionary-db:
	sqlite3 cedict_dictionary/data/cedict_dictionary.db