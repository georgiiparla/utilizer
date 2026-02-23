require 'test_helper'
require 'node'

class TestNode < Minitest::Test
    def test_has_nodes
        n = Node.new({})
        assert_nil n.next_node
        assert_nil n.prev_node
        assert_instance_of Hash, n.props
    end
end