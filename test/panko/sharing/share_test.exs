defmodule Panko.Sharing.ShareTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.Session

  setup do
    {:ok, session} =
      Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "share-test",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  test "creates a share with auto-generated slug", %{session: session} do
    assert {:ok, share} = Panko.Sharing.create_share(session.id)
    assert share.slug != nil
    assert String.length(share.slug) == 8
    assert share.is_shared == true
    assert share.shared_at != nil
  end

  test "unpublish sets is_shared to false", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    {:ok, unpublished} = Panko.Sharing.unpublish_share(share)
    assert unpublished.is_shared == false
    assert unpublished.unshared_at != nil
  end

  test "republish restores sharing with same slug", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    {:ok, unpublished} = Panko.Sharing.unpublish_share(share)
    {:ok, republished} = Panko.Sharing.republish_share(unpublished)
    assert republished.is_shared == true
    assert republished.slug == share.slug
  end

  test "get_share_by_slug finds active share", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    assert {:ok, found} = Panko.Sharing.get_share_by_slug(share.slug)
    assert found.id == share.id
  end

  test "get_share_by_slug returns error for unpublished", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    {:ok, _} = Panko.Sharing.unpublish_share(share)
    assert {:error, _} = Panko.Sharing.get_share_by_slug(share.slug)
  end
end
