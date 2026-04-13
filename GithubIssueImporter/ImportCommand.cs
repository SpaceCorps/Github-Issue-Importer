using Spectre.Console;
using Spectre.Console.Cli;
using System.ComponentModel;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace GithubIssueImporter;

public class ImportSettings : CommandSettings
{
    private static readonly string DefaultConfigPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        ".config", "github-issue-importer", "config.yaml");

    [CommandOption("-c|--config")]
    [Description("Path to config.yaml (default: ~/.config/github-issue-importer/config.yaml)")]
    public string ConfigPath { get; set; } = DefaultConfigPath;
}

public class ImportCommand : AsyncCommand<ImportSettings>
{
    public override async Task<int> ExecuteAsync(CommandContext context, ImportSettings settings)
    {
        if (!File.Exists(settings.ConfigPath))
        {
            AnsiConsole.MarkupLine($"[red]Config file not found:[/] {Markup.Escape(settings.ConfigPath)}");
            AnsiConsole.MarkupLine("[yellow]Create a config.yaml at ~/.config/github-issue-importer/config.yaml[/]");
            AnsiConsole.MarkupLine("[yellow]or specify a path with --config.[/]");
            return 1;
        }

        var yaml = await File.ReadAllTextAsync(settings.ConfigPath);
        var deserializer = new DeserializerBuilder()
            .WithNamingConvention(CamelCaseNamingConvention.Instance)
            .Build();
        var config = deserializer.Deserialize<Config>(yaml);

        if (string.IsNullOrWhiteSpace(config.Inbox))
        {
            AnsiConsole.MarkupLine("[red]inbox path is not set in config.[/]");
            return 1;
        }

        AnsiConsole.Write(new Rule("[blue]GitHub Issue Importer[/]"));

        await AnsiConsole.Status().StartAsync("Checking gh authentication...", async _ =>
        {
            await GhCli.EnsureAuthenticatedAsync();
        });

        Directory.CreateDirectory(config.Inbox);

        var configDir = Path.GetDirectoryName(Path.GetFullPath(settings.ConfigPath)) ?? ".";
        var logPath = Path.Combine(configDir, "import-log.json");
        var log = new ImportLog(logPath);
        AnsiConsole.MarkupLine($"[grey]Import log: {logPath} ({log.Count} entries)[/]");

        var totalImported = 0;

        foreach (var source in config.Sources)
        {
            AnsiConsole.MarkupLine($"\n[bold]Source:[/] {source.Repo}");
            AnsiConsole.MarkupLine($"[grey]Filter:[/] {source.Filter}");

            var issues = await AnsiConsole.Status().StartAsync("Fetching issues...", async _ =>
            {
                return await GhCli.SearchIssuesAsync(source.Repo, source.Filter);
            });

            AnsiConsole.MarkupLine($"[grey]Found {issues.Count} matching issue(s)[/]");

            var table = new Table()
                .Border(TableBorder.Rounded)
                .AddColumn("#")
                .AddColumn("Title")
                .AddColumn("Status");

            foreach (var issue in issues)
            {
                var key = $"{source.Repo}#{issue.Number}";

                if (log.IsImported(key))
                {
                    table.AddRow(
                        issue.Number.ToString(),
                        Markup.Escape(issue.Title),
                        "[grey]already imported[/]"
                    );
                    continue;
                }

                var fileName = $"issue-{issue.Number}.md";
                var filePath = Path.Combine(config.Inbox, fileName);
                await File.WriteAllTextAsync(filePath, $"{issue.Url}\n");

                log.MarkImported(key);
                totalImported++;

                table.AddRow(
                    issue.Number.ToString(),
                    Markup.Escape(issue.Title),
                    "[green]imported[/]"
                );
            }

            AnsiConsole.Write(table);
        }

        AnsiConsole.WriteLine();
        if (totalImported > 0)
            AnsiConsole.MarkupLine($"[green]Imported {totalImported} new issue(s) to {Markup.Escape(config.Inbox)}[/]");
        else
            AnsiConsole.MarkupLine("[grey]No new issues to import.[/]");

        return 0;
    }
}
