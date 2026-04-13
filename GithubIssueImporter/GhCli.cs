using System.Diagnostics;
using System.Text.Json;

namespace GithubIssueImporter;

public record GitHubIssue(int Number, string Title, string Url);

public static class GhCli
{
    public static async Task<List<GitHubIssue>> SearchIssuesAsync(string repo, string filter)
    {
        var repoPath = new Uri(repo).AbsolutePath.TrimStart('/');
        var args = $"issue list --repo {repoPath} --search \"{filter}\" --json number,title,url --limit 200";

        var result = await RunAsync("gh", args);

        if (result.ExitCode != 0)
            throw new InvalidOperationException($"gh failed (exit {result.ExitCode}): {result.StdErr}");

        var issues = JsonSerializer.Deserialize<List<JsonElement>>(result.StdOut) ?? [];

        return issues.Select(e => new GitHubIssue(
            e.GetProperty("number").GetInt32(),
            e.GetProperty("title").GetString() ?? "",
            e.GetProperty("url").GetString() ?? ""
        )).ToList();
    }

    public static async Task EnsureAuthenticatedAsync()
    {
        var result = await RunAsync("gh", "auth status");
        if (result.ExitCode != 0)
            throw new InvalidOperationException("gh CLI is not authenticated. Run 'gh auth login' first.");
    }

    private static async Task<ProcessResult> RunAsync(string fileName, string arguments)
    {
        using var process = new Process();
        process.StartInfo = new ProcessStartInfo
        {
            FileName = fileName,
            Arguments = arguments,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };

        process.Start();
        var stdout = await process.StandardOutput.ReadToEndAsync();
        var stderr = await process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();

        return new ProcessResult(process.ExitCode, stdout, stderr);
    }

    private record ProcessResult(int ExitCode, string StdOut, string StdErr);
}
