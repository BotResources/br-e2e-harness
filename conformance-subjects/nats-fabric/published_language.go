package main

const usersKeyPrefix = "identity/users/"

func userKvKey(userID string) string {
	return usersKeyPrefix + userID
}

type publishedUserWire struct {
	Version   int    `json:"version"`
	Email     string `json:"email"`
	FirstName string `json:"first_name,omitempty"`
	LastName  string `json:"last_name,omitempty"`
	Locale    string `json:"locale,omitempty"`
}

func sampleUser(email, first, last string) publishedUserWire {
	return publishedUserWire{
		Version:   1,
		Email:     email,
		FirstName: first,
		LastName:  last,
		Locale:    "en",
	}
}

const poisonUserValue = `{ "first_name": "no-email", "version": 1`
