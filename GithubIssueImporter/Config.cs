using YamlDotNet.Serialization;

namespace GithubIssueImporter;

public class Config
{
    [YamlMember(Alias = "inbox")]
    public string Inbox { get; set; } = string.Empty;

    [YamlMember(Alias = "sources")]
    public List<Source> Sources { get; set; } = [];
}

public class Source
{
    [YamlMember(Alias = "repo")]
    public string Repo { get; set; } = string.Empty;

    [YamlMember(Alias = "filter")]
    public string Filter { get; set; } = string.Empty;
}
