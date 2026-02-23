require 'test_helper'
require 'linked_list'

class TestLinkedList < Minitest::Test
    def props
        {
                setpos: {
                        x: 1296.0,
                        y: 32.0,
                        z: -103.96875
                },
                setang: {
                        pitch: -28.912655,
                        yaw: -165.958206,
                        roll: 0.0
                },
                metadata: {
                        raw_source: "setpos 1296.000000 32.000000 -103.968750;setang -28.912655 -165.958206 0.000000"
                }
        }
    end
    
    def test_add
        ll = LinkedList.new
        assert_nil ll.head
        
        # 1. Add 6 nodes, injecting an :id into the hash to verify the sequence
        (1..6).each do |i|
            p = props.merge(id: i)
            ll.add(p)
        end
        
        # 2. Verify Head structure
        h1 = ll.head
        assert_instance_of Node, h1
        assert_equal 1, h1.props[:id]
        
        # 3. Walk the whole circle forward: 1 -> 2 -> 3 -> 4 -> 5 -> 6
        current = h1
        (1..6).each do |expected_id|
            # Verify it's a Node
            assert_instance_of Node, current
            
            # Verify the sequence matches
            assert_equal expected_id, current.props[:id]
            
            # Verify the payload
            assert_equal 1296.0, current.props[:setpos][:x]
            assert_equal "setpos 1296.000000 32.000000 -103.968750;setang -28.912655 -165.958206 0.000000", current.props[:metadata][:raw_source]
            
            # Move to next
            current = current.next_node
        end
        
        # 4. Forward Circular Check: After 6 steps, did we loop back to 1?
        assert_equal h1, current
        
        # 5. Backward Circular Check: Does h1 look backwards and see Node 6?
        tail = h1.prev_node
        assert_equal 6, tail.props[:id]
        
        # Does Node 6 look forwards and see Node 1?
        assert_equal h1, tail.next_node
        
        assert_equal 6, ll.length
    end
end