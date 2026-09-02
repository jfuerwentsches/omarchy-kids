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
    entry.sshHostPublicKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIQtest child-host-key";
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
    EXPECT_EQ(
        reloaded.hosts()[0].sshHostPublicKey,
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIQtest child-host-key");
}

// issue #33: the host's real sshd key (distinct from `fingerprint`, which is
// the Control Center's own client-key fingerprint) must round-trip through
// hosts.toml so AgentClient can pin it on later SSH calls.
TEST(HostRegistry, SshHostPublicKeyRoundTripsThroughPersistence) {
    auto path = tempRegistryPath("host-key-roundtrip");
    std::filesystem::remove_all(path.parent_path());
    {
        HostRegistry registry(path);
        registry.addHost(sampleHost("Testkind"));
    }

    HostRegistry reloaded(path);
    ASSERT_EQ(reloaded.hosts().size(), 1u);
    EXPECT_EQ(
        reloaded.hosts()[0].sshHostPublicKey,
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIQtest child-host-key");
}

// A host paired before this field existed has no ssh_host_public_key key in
// its TOML table at all — must load as an empty string (AgentClient's
// documented TOFU fallback), not fail to parse the rest of the entry.
TEST(HostRegistry, MissingSshHostPublicKeyLoadsAsEmptyString) {
    auto path = tempRegistryPath("host-key-missing");
    std::filesystem::remove_all(path.parent_path());
    std::filesystem::create_directories(path.parent_path());
    std::ofstream(path) << R"(
[[hosts]]
name = "Altkind"
hostname = "192.168.1.50"
ssh_port = 22
username = "kid"
key_path = "/home/parent/.ssh/kid_key"
fingerprint = "SHA256:def456"
paired_at = "2026-08-29T00:00:00Z"
)";

    HostRegistry registry(path);

    ASSERT_EQ(registry.hosts().size(), 1u);
    EXPECT_EQ(registry.hosts()[0].sshHostPublicKey, "");
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
