package main

import "github.com/google/uuid"

const (
	usersKeyPrefix  = "identity/users/"
	groupsKeyPrefix = "identity/groups/"
	metaKey         = "identity/_meta"

	directoryMetaVersion = 1

	neutralExtensionKey = "x_custom"
)

func userKVKey(userID uuid.UUID) string {
	return usersKeyPrefix + userID.String()
}

func groupKVKey(groupID uuid.UUID) string {
	return groupsKeyPrefix + groupID.String()
}

func publishedUserCore(email string, firstName *string, lastName *string) map[string]any {
	return map[string]any{
		"email":      email,
		"first_name": firstName,
		"last_name":  lastName,
	}
}

func publishedUserWithExtension(email string, firstName *string, lastName *string, extension any) map[string]any {
	wire := publishedUserCore(email, firstName, lastName)
	wire[neutralExtensionKey] = extension
	return wire
}

func publishedGroupCore(name string, memberIDs []uuid.UUID) map[string]any {
	members := make([]string, len(memberIDs))
	for i, id := range memberIDs {
		members[i] = id.String()
	}
	return map[string]any{
		"name":       name,
		"member_ids": members,
	}
}

func publishedGroupWithExtension(name string, memberIDs []uuid.UUID, extension any) map[string]any {
	wire := publishedGroupCore(name, memberIDs)
	wire[neutralExtensionKey] = extension
	return wire
}

func directoryMeta(entities []string) map[string]any {
	return map[string]any{
		"version":  directoryMetaVersion,
		"entities": entities,
	}
}
