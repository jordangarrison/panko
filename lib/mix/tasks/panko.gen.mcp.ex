defmodule Mix.Tasks.Panko.Gen.Mcp do
  @moduledoc """
  Generates .mcp.json with the correct port for the running Phoenix server.

  ## Usage

      mix panko.gen.mcp
      mix panko.gen.mcp --port 4001
  """
  use Mix.Task

  @impl Mix.Task
  def run(args) do
    {opts, _, _} = OptionParser.parse(args, strict: [port: :integer])

    port = opts[:port] || 4000

    content =
      Jason.encode!(
        %{
          "mcpServers" => %{
            "tidewave" => %{
              "type" => "http",
              "url" => "http://localhost:#{port}/tidewave/mcp"
            }
          }
        },
        pretty: true
      )

    File.write!(".mcp.json", content)
    Mix.shell().info("Generated .mcp.json for port #{port}")
  end
end
