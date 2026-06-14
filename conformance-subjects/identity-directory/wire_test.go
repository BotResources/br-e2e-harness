package main

import (
	"encoding/json"
	"reflect"
	"testing"

	"github.com/google/uuid"
)

func TestKVKeysUseFrozenPrefixes(t *testing.T) {
	id := uuid.MustParse("01938c1f-0000-7000-8000-000000000001")
	if got, want := userKVKey(id), "identity/users/01938c1f-0000-7000-8000-000000000001"; got != want {
		t.Fatalf("userKVKey = %q, want %q", got, want)
	}
	if got, want := groupKVKey(id), "identity/groups/01938c1f-0000-7000-8000-000000000001"; got != want {
		t.Fatalf("groupKVKey = %q, want %q", got, want)
	}
	if metaKey != "identity/_meta" {
		t.Fatalf("metaKey = %q, want identity/_meta", metaKey)
	}
}

func TestPublishedUserMatchesGoldenShape(t *testing.T) {
	got, err := json.Marshal(publishedUserWithExtension(
		"ada@example.com", strptr("Ada"), strptr("Lovelace"),
		map[string]any{"nested": "value"},
	))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "x_custom": {"nested": "value"}
    }`
	assertJSONEqual(t, got, []byte(golden))
}

func TestPublishedUserCoreKeysAreExactlyTheContract(t *testing.T) {
	got, err := json.Marshal(publishedUserCore("x@y.z", strptr("X"), strptr("Y")))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var asMap map[string]any
	if err := json.Unmarshal(got, &asMap); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	want := map[string]struct{}{"email": {}, "first_name": {}, "last_name": {}}
	for k := range asMap {
		if _, ok := want[k]; !ok {
			t.Fatalf("unexpected core user key %q", k)
		}
	}
	for k := range want {
		if _, ok := asMap[k]; !ok {
			t.Fatalf("missing required core user key %q", k)
		}
	}
}

func TestPublishedUserCoreEmitsNullNamesNotOmitted(t *testing.T) {
	got, err := json.Marshal(publishedUserCore("solo@example.com", nil, nil))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var asMap map[string]any
	if err := json.Unmarshal(got, &asMap); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if _, present := asMap["first_name"]; !present {
		t.Fatalf("first_name must be present (null), not omitted: %s", got)
	}
	if asMap["first_name"] != nil {
		t.Fatalf("first_name = %v, want null", asMap["first_name"])
	}
	if _, present := asMap["last_name"]; !present {
		t.Fatalf("last_name must be present (null), not omitted: %s", got)
	}
}

func TestNeutralExtensionRidesFlatAlongsideCore(t *testing.T) {
	got, err := json.Marshal(publishedUserWithExtension(
		"x@y.z", strptr("X"), strptr("Y"), "flat-value",
	))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var asMap map[string]any
	if err := json.Unmarshal(got, &asMap); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if asMap[neutralExtensionKey] != "flat-value" {
		t.Fatalf("extension %q not flat at top level, got %v", neutralExtensionKey, asMap)
	}
	if _, ok := asMap["extensions"]; ok {
		t.Fatalf("extensions must flatten, never nest under an 'extensions' key: %s", got)
	}
}

func TestPublishedGroupMatchesGoldenShape(t *testing.T) {
	members := []uuid.UUID{
		uuid.MustParse("01938c1f-0000-7000-8000-000000000001"),
		uuid.MustParse("01938c1f-0000-7000-8000-000000000002"),
	}
	got, err := json.Marshal(publishedGroupWithExtension("engineering", members, false))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{
      "name": "engineering",
      "member_ids": [
        "01938c1f-0000-7000-8000-000000000001",
        "01938c1f-0000-7000-8000-000000000002"
      ],
      "x_custom": false
    }`
	assertJSONEqual(t, got, []byte(golden))
}

func TestPublishedGroupCoreKeysAreExactlyTheContract(t *testing.T) {
	got, err := json.Marshal(publishedGroupCore("g", nil))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var asMap map[string]any
	if err := json.Unmarshal(got, &asMap); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	want := map[string]struct{}{"name": {}, "member_ids": {}}
	for k := range asMap {
		if _, ok := want[k]; !ok {
			t.Fatalf("unexpected core group key %q", k)
		}
	}
	for k := range want {
		if _, ok := asMap[k]; !ok {
			t.Fatalf("missing required core group key %q", k)
		}
	}
}

func TestPublishedGroupMemberIDsIsAlwaysAnArray(t *testing.T) {
	got, err := json.Marshal(publishedGroupCore("empty", nil))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var p struct {
		MemberIDs json.RawMessage `json:"member_ids"`
	}
	if err := json.Unmarshal(got, &p); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	var arr []any
	if err := json.Unmarshal(p.MemberIDs, &arr); err != nil {
		t.Fatalf("member_ids must be a JSON array: %v (%s)", err, p.MemberIDs)
	}
	if len(arr) != 0 {
		t.Fatalf("expected empty member_ids array, got %v", arr)
	}
}

func TestDirectoryMetaMatchesGoldenShape(t *testing.T) {
	got, err := json.Marshal(directoryMeta([]string{"users", "groups"}))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{"version": 1, "entities": ["users", "groups"]}`
	assertJSONEqual(t, got, []byte(golden))
}

func TestDirectoryMetaUsersOnlyAutoDegrades(t *testing.T) {
	got, err := json.Marshal(directoryMeta([]string{"users"}))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{"version": 1, "entities": ["users"]}`
	assertJSONEqual(t, got, []byte(golden))
}

func TestCanonicalSnapshotIsWellFormed(t *testing.T) {
	snapshot := canonicalSnapshot()
	if snapshot.Meta.Key != metaKey {
		t.Fatalf("meta key = %q, want %q", snapshot.Meta.Key, metaKey)
	}
	if len(snapshot.Users) == 0 {
		t.Fatal("snapshot must publish at least one user")
	}
	if len(snapshot.Groups) == 0 {
		t.Fatal("snapshot must publish at least one group")
	}
	for _, u := range snapshot.Users {
		if got := u.Key[:len(usersKeyPrefix)]; got != usersKeyPrefix {
			t.Fatalf("user entry key %q lacks prefix %q", u.Key, usersKeyPrefix)
		}
		if _, err := uuid.Parse(u.Key[len(usersKeyPrefix):]); err != nil {
			t.Fatalf("user entry key %q suffix is not a uuid: %v", u.Key, err)
		}
	}
	for _, g := range snapshot.Groups {
		if got := g.Key[:len(groupsKeyPrefix)]; got != groupsKeyPrefix {
			t.Fatalf("group entry key %q lacks prefix %q", g.Key, groupsKeyPrefix)
		}
		if _, err := uuid.Parse(g.Key[len(groupsKeyPrefix):]); err != nil {
			t.Fatalf("group entry key %q suffix is not a uuid: %v", g.Key, err)
		}
	}
}

func assertJSONEqual(t *testing.T, a, b []byte) {
	t.Helper()
	var am, bm any
	if err := json.Unmarshal(a, &am); err != nil {
		t.Fatalf("unmarshal got: %v\n%s", err, a)
	}
	if err := json.Unmarshal(b, &bm); err != nil {
		t.Fatalf("unmarshal golden: %v", err)
	}
	if !reflect.DeepEqual(am, bm) {
		ap, _ := json.MarshalIndent(am, "", "  ")
		bp, _ := json.MarshalIndent(bm, "", "  ")
		t.Fatalf("JSON mismatch.\n--- got ---\n%s\n--- want ---\n%s", ap, bp)
	}
}
