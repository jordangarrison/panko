defmodule Panko.Sharing.Workers.ShareReaperTest do
  use Panko.DataCase, async: true
  use Oban.Testing, repo: Panko.Repo

  alias Panko.Sharing.Share
  alias Panko.Sharing.Workers.ShareReaper

  setup do
    {:ok, session} =
      Panko.Sessions.Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "reaper-test",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  defp create_share_with_expiry(session_id, expires_at) do
    Share
    |> Ash.Changeset.for_create(:create, %{
      session_id: session_id,
      expires_at: expires_at
    })
    |> Ash.create!()
  end

  test "deactivates expired shares", %{session: session} do
    share = create_share_with_expiry(session.id, ~U[2020-01-01 00:00:00Z])
    assert share.is_shared == true

    assert :ok = perform_job(ShareReaper, %{})

    assert {:error, _} = Panko.Sharing.get_share_by_slug(share.slug)
  end

  test "leaves non-expired shares alone", %{session: session} do
    share = create_share_with_expiry(session.id, ~U[2099-01-01 00:00:00Z])

    assert :ok = perform_job(ShareReaper, %{})

    assert {:ok, found} = Panko.Sharing.get_share_by_slug(share.slug)
    assert found.is_shared == true
  end

  test "leaves shares without expiry alone", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)

    assert :ok = perform_job(ShareReaper, %{})

    assert {:ok, found} = Panko.Sharing.get_share_by_slug(share.slug)
    assert found.is_shared == true
  end
end
