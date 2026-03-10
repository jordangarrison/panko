defmodule Panko.Sessions.Parsers.ClaudeCodeTest do
  use ExUnit.Case, async: true

  alias Panko.Sessions.Parsers.ClaudeCode

  @fixtures_dir Path.join([__DIR__, "../../../fixtures"])

  describe "can_parse?/1" do
    test "returns true for .jsonl files" do
      assert ClaudeCode.can_parse?("/path/to/session.jsonl")
    end

    test "returns false for other files" do
      refute ClaudeCode.can_parse?("/path/to/file.json")
      refute ClaudeCode.can_parse?("/path/to/file.txt")
    end
  end

  describe "parse/1" do
    test "parses a simple session" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      assert {:ok, attrs} = ClaudeCode.parse(path)

      assert attrs.external_id == "test-abc-123"
      assert attrs.source_type == :claude_code
      assert attrs.source_path == path
      assert attrs.project == "/home/user/my-project"
      assert attrs.title == "List the files in the current directory"
      assert %DateTime{} = attrs.started_at
    end

    test "extracts blocks in order" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)

      assert length(attrs.blocks) >= 3

      types = Enum.map(attrs.blocks, & &1.block_type)
      assert :user_prompt in types
      assert :assistant_response in types
      assert :tool_call in types
    end

    test "extracts tool call metadata" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)

      tool_block = Enum.find(attrs.blocks, &(&1.block_type == :tool_call))
      assert tool_block != nil
      assert tool_block.metadata["name"] == "Bash"
      assert tool_block.metadata["input"] == %{"command" => "ls -la"}
    end

    test "returns error for non-existent file" do
      assert {:error, _} = ClaudeCode.parse("/nonexistent/file.jsonl")
    end
  end
end
