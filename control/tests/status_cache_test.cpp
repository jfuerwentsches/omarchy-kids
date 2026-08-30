#include "omarchy-kids-control/status_cache.h"

#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>
#include <sstream>

using omarchy_kids::control::HostStatus;
using omarchy_kids::control::StatusCache;

namespace {

std::string readFile(const std::filesystem::path& path) {
    std::ifstream in(path);
    std::ostringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

// Each test gets its own throwaway directory under /tmp so runs never
// interfere with each other or with a real ~/.config/omarchy-kids-control.
std::filesystem::path tempCachePath(const std::string& caseName) {
    return std::filesystem::temp_directory_path()
        / ("omarchy-kids-control-test-" + caseName) / "status-cache.json";
}

} // namespace

TEST(StatusCache, WritesEmptyArrayWhenNoEntries) {
    auto path = tempCachePath("empty");
    std::filesystem::remove_all(path.parent_path());
    StatusCache cache(path);

    cache.write();

    ASSERT_TRUE(std::filesystem::exists(path));
    EXPECT_EQ(readFile(path), "[\n]\n");
}

TEST(StatusCache, CreatesParentDirectoryIfMissing) {
    auto path = tempCachePath("nested-dir");
    std::filesystem::remove_all(path.parent_path());
    StatusCache cache(path);

    cache.write();

    EXPECT_TRUE(std::filesystem::exists(path));
}

TEST(StatusCache, WritesFieldsForEachHost) {
    auto path = tempCachePath("fields");
    std::filesystem::remove_all(path.parent_path());
    StatusCache cache(path);

    HostStatus online;
    online.name = "Lea";
    online.online = true;
    online.checkedAt = "2026-08-30T15:00:00Z";
    cache.set(online);

    HostStatus offline;
    offline.name = "Finn";
    offline.online = false;
    offline.checkedAt = "2026-08-30T15:00:01Z";
    offline.lastError = "unreachable over SSH";
    cache.set(offline);

    cache.write();

    const std::string content = readFile(path);
    EXPECT_NE(content.find(R"("name": "Lea")"), std::string::npos);
    EXPECT_NE(content.find(R"("online": true)"), std::string::npos);
    EXPECT_NE(content.find(R"("name": "Finn")"), std::string::npos);
    EXPECT_NE(content.find(R"("online": false)"), std::string::npos);
    EXPECT_NE(content.find(R"("lastError": "unreachable over SSH")"), std::string::npos);
}

// The only other reader of this file is QML's JSON.parse() (see the
// Quickshell plugin) — an unescaped quote/backslash/control character in a
// parent-chosen child name would corrupt the surrounding JSON and break
// that parse.
TEST(StatusCache, EscapesSpecialCharactersInHostName) {
    auto path = tempCachePath("escaping");
    std::filesystem::remove_all(path.parent_path());
    StatusCache cache(path);

    HostStatus status;
    status.name = "Quote\"Backslash\\Newline\nTab\t";
    status.online = true;
    status.checkedAt = "2026-08-30T15:00:00Z";
    cache.set(status);

    cache.write();

    const std::string content = readFile(path);
    EXPECT_NE(content.find(R"(Quote\"Backslash\\Newline\nTab\t)"), std::string::npos);
}

// Each poll run builds a fresh StatusCache and checks every currently-
// paired host (see poll_runner.cpp) — write() must fully replace the file,
// not merge with whatever an earlier run left behind, or an unpaired/
// removed child would linger in the cache forever.
TEST(StatusCache, EachWriteFullyReplacesPreviousContents) {
    auto path = tempCachePath("overwrite");
    std::filesystem::remove_all(path.parent_path());

    StatusCache first(path);
    HostStatus stale;
    stale.name = "Stale";
    stale.online = true;
    stale.checkedAt = "2026-08-30T00:00:00Z";
    first.set(stale);
    first.write();

    StatusCache second(path);
    second.write();

    EXPECT_EQ(readFile(path), "[\n]\n");
}

TEST(StatusCache, DoesNotLeaveTempFileBehind) {
    auto path = tempCachePath("no-tmp-leftover");
    std::filesystem::remove_all(path.parent_path());
    StatusCache cache(path);

    cache.write();

    EXPECT_FALSE(std::filesystem::exists(path.string() + ".tmp"));
}
