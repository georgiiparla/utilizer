require "test_helper"
require "playlist_parser"

class TestPlaylistParser < Minitest::Test
  def display_linked_list(linked_list)
    current = linked_list.head
    return if current.nil?

    loop do
      p current.props
      current = current.next_node
      break if current == linked_list.head
    end
  end

  def test_parser
    ll = LinkedList.new

    p = PlaylistParser.new(CONFIG[:playlists_folder])
    assert_instance_of PlaylistParser, p

    p.apply("mirage.csv", ll)
    assert_equal 4, ll.length

    display_linked_list(ll)
  end
end
