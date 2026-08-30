#include "omarchy-kids-control/host_registry.h"

#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>

using omarchy_kids::control::HostEntry;
using omarchy_kids::control::HostRegistry;

namespace {

std::filesystem::path tempRegistryPath(const std::string& caseName) {
    return std::filesystem::temp_directory_path()
        / ("omarchy-kids-control-test-" + caseName) / "hosts.toml";
}

HostEntry sampleHost(const std::string& name) {
    HostEntry entry;
    entry.name = name;
    entry.hostname = "192.168.1.42";
    entry.sshPort = 22;
    entry.username = "kid";
    entry.keyPath = "/home/parent/.ssh/kid_key";
    entry.fingerprint = "SHA256:abc123";
    entry.pairedAt = "2026-08-30T00:00:00Z";
    return entry;
}

} // namespace

TEST(HostRegistry, StartsEmptyWhenNoFileExists) {
    auto path = tempRegistryPath("missing");
    std::filesystem::remove_all(path.parent_path());

    HostRegistry registry(path);

    EXPECT_TRUE(registry.hosts().empty());
}

TEST(HostRegistry, AddHostPersistsAcrossReload) {
    auto path = tempRegistryPath("persist");
    std::filesystem::remove_all(path.parent_path());
    {
        HostRegistry registry(path);
        registry.addHost(sampleHost("Testkind"));
    }

    HostRegistry reloaded(path);

    ASSERT_EQ(reloaded.hosts().size(), 1u);
    EXPECT_EQ(reloaded.hosts()[0].name, "Testkind");
    EXPECT_EQ(reloaded.hosts()[0].hostname, "192.168.1.42");
    EXPECT_EQ(reloaded.hosts()[0].fingerprint, "SHA256:abc123");
}

// Re-pairing the same child (e.g. after a DHCP lease changed its address)
// must update the existing entry, not accumulate duplicates — see
// addHost()'s doc comment.
TEST(HostRegistry, ReAddingSameNameUpdatesInPlaceRatherThanDuplicating) {
    auto path = tempRegistryPath("repair");
    std::filesystem::remove_all(path.parent_path());
    HostRegistry registry(path);
    registry.addHost(sampleHost("Testkind"));

    HostEntry updated = sampleHost("Testkind");
    updated.hostname = "192.168.1.99";
    registry.addHost(updated);

    ASSERT_EQ(registry.hosts().size(), 1u);
    EXPECT_EQ(registry.hosts()[0].hostname, "192.168.1.99");
}

TEST(HostRegistry, TreatsMalformedFileAsEmptyRatherThanCrashing) {
    auto path = tempRegistryPath("malformed");
    std::filesystem::remove_all(path.parent_path());
    std::filesystem::create_directories(path.parent_path());
    std::ofstream(path) << "this is not valid toml {{{";

    HostRegistry registry(path);

    EXPECT_TRUE(registry.hosts().empty());
}
