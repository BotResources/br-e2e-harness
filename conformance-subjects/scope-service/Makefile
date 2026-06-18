BINARY := scope-service

.PHONY: build vet test fmt check guard clean

build:
	go build -o $(BINARY) .

vet:
	go vet ./...

test: vet
	go test ./...

fmt:
	gofmt -w .

guard:
	@grep -rn --exclude=Makefile "identity.cmd.\|identity.evt." . && (echo DEAD GRAMMAR && exit 1) || true

check: fmt vet test guard

clean:
	rm -f $(BINARY)
