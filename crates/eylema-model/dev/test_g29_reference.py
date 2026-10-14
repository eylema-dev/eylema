"""Independent anchors: wrong byte bijection, merge order or typed rendering must fail."""
import unittest
from g29_reference import alphabet, bpe, pattern, render

class References(unittest.TestCase):
    def test_byte_alphabet_is_reversible(self):
        a=alphabet();self.assertEqual(len(set(a)),256);self.assertEqual(a[32],"Ġ");self.assertEqual(a[10],"Ċ")
    def test_overlap_and_ranks(self):
        ranks={("a","b"):0,("b","a"):1,("ab","a"):2}
        self.assertEqual(bpe("aba",ranks),["aba"]);self.assertEqual(bpe("abab",ranks),["ab","ab"])
    def test_unicode_split_literal(self):
        p=pattern();self.assertEqual(p.findall(" Hello world  "),[" Hello"," world","  "])
        self.assertEqual(p.findall("123\r\n"),["1","2","3","\r\n"])
        self.assertEqual(p.findall("I'M 'ſ"),["I","'M"," '","ſ"]);self.assertEqual(p.findall("'ſ"),["'ſ"])
    def test_typed_render_literal(self):
        self.assertEqual(render([("user","Hi")]),"<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n")

if __name__=="__main__":unittest.main()
