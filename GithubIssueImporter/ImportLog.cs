using System.Text.Json;

namespace GithubIssueImporter;

public class ImportLog
{
    private readonly string _path;
    private HashSet<string> _imported;

    public ImportLog(string path)
    {
        _path = path;
        _imported = Load();
    }

    public bool IsImported(string issueId) => _imported.Contains(issueId);

    public void MarkImported(string issueId)
    {
        _imported.Add(issueId);
        Save();
    }

    public int Count => _imported.Count;

    private HashSet<string> Load()
    {
        if (!File.Exists(_path))
            return [];

        var json = File.ReadAllText(_path);
        return JsonSerializer.Deserialize<HashSet<string>>(json) ?? [];
    }

    private void Save()
    {
        var json = JsonSerializer.Serialize(_imported, new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(_path, json);
    }
}
