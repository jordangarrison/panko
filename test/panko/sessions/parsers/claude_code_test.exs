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

  describe "parse/1 complex session" do
    test "skips progress and file-history-snapshot records" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)
      types = Enum.map(attrs.blocks, & &1.block_type)
      refute :progress in types
      refute :file_history_snapshot in types
    end

    test "extracts thinking blocks" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)

      thinking = Enum.find(attrs.blocks, &(&1.block_type == :thinking))
      assert thinking != nil
      assert thinking.content =~ "think about how to structure"
    end

    test "extracts file edit blocks" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)
      edit = Enum.find(attrs.blocks, &(&1.block_type == :file_edit))
      assert edit != nil
      assert edit.metadata["path"] != nil
    end

    test "extracts sub_agent_spawn blocks and agents" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)

      spawn_block = Enum.find(attrs.blocks, &(&1.block_type == :sub_agent_spawn))
      assert spawn_block != nil

      assert length(attrs.sub_agents) >= 1
      agent = hd(attrs.sub_agents)
      assert agent.agent_type != nil
      assert agent.status == :running
    end

    test "file edit metadata includes tool name and path" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)
      edit = Enum.find(attrs.blocks, &(&1.block_type == :file_edit))
      assert edit.metadata["name"] == "Write"
      assert edit.metadata["path"] == "/home/user/complex-project/lib/helper.ex"
    end

    test "sub_agent has correct attributes from Agent tool" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)
      agent = hd(attrs.sub_agents)
      assert agent.agent_type == "explore"
      assert agent.description == "Explore the codebase structure"
      assert agent.prompt == "List all modules and their public functions"
      assert agent.external_id == "toolu_agent_1"
    end

    test "blocks are assigned sequential positions" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)
      positions = Enum.map(attrs.blocks, & &1.position)
      assert positions == Enum.sort(positions)
      assert Enum.uniq(positions) == positions
    end

    test "tool results from user messages are skipped as blocks" do
      path = Path.join(@fixtures_dir, "complex_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)
      # The tool_result user message (u2) should not produce user_prompt blocks
      # Only the first user message should produce a user_prompt
      user_blocks = Enum.filter(attrs.blocks, &(&1.block_type == :user_prompt))
      assert length(user_blocks) == 1
      assert hd(user_blocks).content == "Create a helper module and explore the codebase"
    end

    test "handles empty file" do
      path = Path.join(@fixtures_dir, "empty_session.jsonl")
      File.write!(path, "")
      assert {:ok, attrs} = ClaudeCode.parse(path)
      assert attrs.blocks == []
      File.rm!(path)
    end
  end
end
