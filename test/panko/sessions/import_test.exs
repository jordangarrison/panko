defmodule Panko.Sessions.ImportTest do
  use Panko.DataCase, async: true

  @fixtures_dir Path.join([__DIR__, "../../fixtures"])

  describe "import_from_file" do
    test "imports a session from JSONL file" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      assert {:ok, session} = Panko.Sessions.import_from_file(path)

      assert session.external_id == "test-abc-123"
      assert session.source_type == :claude_code

      session = Ash.load!(session, [:blocks, :sub_agents, :block_count])
      assert session.block_count > 0
      assert length(session.blocks) > 0
    end

    test "upserts on reimport (same external_id)" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")

      assert {:ok, session1} = Panko.Sessions.import_from_file(path)
      assert {:ok, session2} = Panko.Sessions.import_from_file(path)

      assert session1.id == session2.id
    end

    test "returns error for unparseable file" do
      assert {:error, _} = Panko.Sessions.import_from_file("/tmp/nonexistent.jsonl")
    end
  end
end
