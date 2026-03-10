defmodule Panko.Sessions.SubAgentTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.{Session, SubAgent}

  setup do
    {:ok, session} =
      Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "subagent-test-session",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  test "creates a sub_agent", %{session: session} do
    assert {:ok, agent} =
             SubAgent
             |> Ash.Changeset.for_create(:create, %{
               session_id: session.id,
               external_id: "toolu_abc123",
               agent_type: "Explore",
               description: "Search for patterns",
               prompt: "Find all GenServer modules",
               status: :completed,
               result: "Found 3 GenServers",
               spawned_at: ~U[2026-03-09 12:00:05Z],
               completed_at: ~U[2026-03-09 12:00:10Z]
             })
             |> Ash.create()

    assert agent.agent_type == "Explore"
    assert agent.status == :completed
  end
end
