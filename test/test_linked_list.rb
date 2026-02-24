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
                description: "Instant window smoke",
                metadata: {
                        raw_source: "setpos 1296.000000 32.000000 -103.968750;setang -28.912655 -165.958206 0.000000,Instant window smoke"
                }
        }
    end
    
    def test_add
        ll = LinkedList.new
        assert_nil ll.head
        
        (1..6).each do |i|
            p = props.merge(id: i)
            ll.add(p)
        end
        
        h1 = ll.head
        assert_instance_of Node, h1
        assert_equal 1, h1.props[:id]
        
        current = h1
        (1..6).each do |expected_id|
            assert_instance_of Node, current
            
            assert_equal expected_id, current.props[:id]
            
            assert_equal 1296.0, current.props[:setpos][:x]
            assert_equal "setpos 1296.000000 32.000000 -103.968750;setang -28.912655 -165.958206 0.000000,Instant window smoke", current.props[:metadata][:raw_source]
            
            current = current.next_node
        end
        
        assert_equal h1, current
        
        tail = h1.prev_node
        assert_equal 6, tail.props[:id]
        
        assert_equal h1, tail.next_node
        
        assert_equal 6, ll.length
    end
end