require 'test_helper'
require 'csv'

class TestPlaylistParser < Minitest::Test
    def test_read_csv
        csv_table_obj = PlaylistParser.read_csv("mirage.csv")
        assert_instance_of CSV::Table, csv_table_obj
    end
    
    def test_apply_csv_table
        ll = LinkedList.new
        csv_table_obj = PlaylistParser.read_csv("mirage.csv")
        assert_equal 4, csv_table_obj.length
        
        PlaylistParser.apply(csv_table_obj, ll)
        assert_equal 4, ll.length
    end
end