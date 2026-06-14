BINARY := identity-directory

.PHONY: build vet test fmt check clean

build:
	go build -o $(BINARY) .

vet:
	go vet ./...

test: vet
	go test ./...

fmt:
	gofmt -w .

check: fmt vet test

clean:
	rm -f $(BINARY)
