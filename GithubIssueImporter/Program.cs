using GithubIssueImporter;
using Spectre.Console.Cli;

var app = new CommandApp<ImportCommand>();
app.Configure(config =>
{
    config.SetApplicationName("github-issue-importer");
    config.SetApplicationVersion("1.0.0");
});

return await app.RunAsync(args);
