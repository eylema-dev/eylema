"""Build the Eylema brand brochure; editorial tooling, never application runtime.

Read assets/eylema-brochure/copy.{en,tr}.json and use the existing logo PNGs.
Pass --font-dir for a licensed Arial installation. No network calls or installs.
Optional --verify extracts text and renders all pages to ignored .cache storage.
"""
import argparse
import hashlib
import importlib.metadata
import json
import platform
import re
from pathlib import Path
from xml.sax.saxutils import escape

from reportlab.graphics import renderPDF
from reportlab.graphics.barcode.qr import QrCodeWidget
from reportlab.graphics.shapes import Drawing
from reportlab.lib.colors import HexColor
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.pdfdoc import PDFString, PDFtrue
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen.canvas import Canvas
from reportlab.platypus import Paragraph

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "assets/eylema-brochure"
BRAND = ROOT / "assets/eylema-logo"
WIDTH, HEIGHT = A4
INK = "#17252B"
ORANGE = "#EE5638"
PAPER = "#F7F6F2"
MUTED = "#526066"
LIGHT = "#D7DFDF"
WHITE = "#FFFFFF"
CORE_REV = "7e64118196845814d9e8de531fb817c9f14fde5a"
DOCS_REV = "4e8f91ce6515bb7d0a02aab89c97cfca8e086a4f"
SOURCES = {
    "Core status": f"https://github.com/eylema-dev/eylema/blob/{CORE_REV}/README.md",
    "Typed task contracts": f"https://github.com/eylema-dev/eylema/blob/{CORE_REV}/docs/en/typed-contracts.md",
    "Architecture": f"https://github.com/eylema-dev/eylema-docs-en/blob/{DOCS_REV}/docs/architecture.md",
    "API and oversight": f"https://github.com/eylema-dev/eylema-docs-en/blob/{DOCS_REV}/docs/api-reference.md",
    "Roadmap": f"https://github.com/eylema-dev/eylema-docs-en/blob/{DOCS_REV}/docs/roadmap.md",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Brochure:
    def __init__(self, copy, path):
        self.copy = copy
        self.page = 0
        self.boxes = []
        self.canvas = Canvas(str(path), pagesize=A4, pageCompression=1, invariant=1)
        self.canvas.setTitle(copy["title"])
        self.canvas.setAuthor("Eylema")
        self.canvas.setSubject("Early-development vision brochure / " + copy["edition"])
        self.canvas.setCreator("Eylema brochure builder / ReportLab")
        self.canvas.setViewerPreference("DisplayDocTitle", PDFtrue)
        self.canvas._doc.Catalog.Lang = PDFString(copy["language"])

    def rect(self, x, top, width, height, color):
        self.canvas.setFillColor(HexColor(color))
        self.canvas.rect(x, HEIGHT - top - height, width, height, stroke=0, fill=1)

    def line(self, x1, y1, x2, y2, color=ORANGE, weight=1):
        self.canvas.setStrokeColor(HexColor(color))
        self.canvas.setLineWidth(weight)
        self.canvas.line(x1, HEIGHT - y1, x2, HEIGHT - y2)

    def box(self, x, top, width, height, content):
        if min(x, top) < 0 or x + width > WIDTH - 25 or top + height > HEIGHT - 15:
            raise ValueError(f"Page {self.page}: text outside safe page area: {content}")
        self.boxes.append({"page": self.page, "x": x, "top": top,
                           "width": width, "height": height, "text": content})

    def text(self, content, x, top, size=12, font="Regular", color=INK):
        width = pdfmetrics.stringWidth(content, font, size)
        self.box(x, top, width, size * 1.15, content)
        self.canvas.setFont(font, size)
        self.canvas.setFillColor(HexColor(color))
        self.canvas.drawString(x, HEIGHT - top - size, content)

    def paragraph(self, content, x, top, width, size=11.5, leading=16,
                  color=INK, max_height=80, font="Regular"):
        style = ParagraphStyle("Body", fontName=font, fontSize=size,
                               leading=leading, textColor=HexColor(color),
                               splitLongWords=False, allowWidows=0, allowOrphans=0)
        p = Paragraph(escape(content), style)
        _, height = p.wrap(width, HEIGHT)
        if height > max_height:
            raise ValueError(f"Page {self.page}: paragraph is {height}pt, limit {max_height}pt: {content}")
        self.box(x, top, width, height, content)
        p.drawOn(self.canvas, x, HEIGHT - top - height)
        return height

    def title(self, lines, top, size=36, leading=42, first_color=INK, last_color=None):
        for i, line in enumerate(lines):
            color = last_color if last_color and i == len(lines) - 1 else first_color
            self.text(line, 42, top + i * leading, size, "Black", color)

    def logo(self, top=32, width=127, dark=False):
        name = "logo-dark.png" if dark else "logo.png"
        from PIL import Image
        with Image.open(BRAND / name) as im:
            height = width * im.height / im.width
        self.canvas.drawImage(str(BRAND / name), 38, HEIGHT - top - height,
                              width=width, height=height, mask="auto")

    def start_page(self, kicker=None, dark=False):
        self.page += 1
        self.rect(0, 0, WIDTH, HEIGHT, INK if dark else PAPER)
        self.logo(dark=dark)
        if kicker:
            self.text(kicker, 42, 93, 8.6, "Bold", ORANGE if dark else MUTED)
        self.canvas.bookmarkPage(f"page-{self.page}")

    def end_page(self, dark=False):
        color = LIGHT if dark else MUTED
        self.line(42, 796, WIDTH - 42, 796, "#3D4B50" if dark else "#CCD0CA", 0.65)
        self.text(self.copy["footer"], 42, 809, 7.5, "Regular", color)
        self.text(f"0{self.page} / 04", WIDTH - 76, 809, 8, "Bold", color)
        self.canvas.showPage()

    def arrow(self, x1, y1, x2, y2):
        self.line(x1, y1, x2, y2, ORANGE, 1.6)
        import math
        angle = math.atan2(y2 - y1, x2 - x1)
        for offset in [-0.6, 0.6]:
            self.line(x2, y2, x2 - 7 * math.cos(angle + offset),
                      y2 - 7 * math.sin(angle + offset), ORANGE, 1.6)

    def cover(self):
        d = self.copy["cover"]
        self.start_page(dark=True)
        self.text(d["kicker"], 42, 131, 10, "Bold", ORANGE)
        self.title(d["headline"], 168, 65, 73, WHITE, ORANGE)
        self.text(d["subhead"], 44, 334, 31, "Bold", WHITE)
        self.paragraph(d["intro"], 44, 401, 440, 14, 21, LIGHT, 90)
        self.line(44, 566, 44, 669, ORANGE, 3)
        for i, line in enumerate(d["statement"]):
            self.text(line, 59, 579 + i * 27, 19, "Bold", WHITE)
        self.paragraph(d["caption"], 59, 644, 250, 10.4, 15, LIGHT, 30)
        self.canvas.drawImage(str(BRAND / "logo-icon.png"), 382, HEIGHT - 677,
                              width=170, height=170, mask="auto")
        self.text(d["status"], 44, 746, 8.5, "Bold", LIGHT)
        self.end_page(dark=True)

    def why(self):
        d = self.copy["why"]
        self.start_page(d["kicker"])
        self.title(d["headline"], 130, 35, 41)
        self.paragraph(d["intro"], 44, 232, 501, 12, 18, MUTED, 65)
        for i, item in enumerate(d["reasons"]):
            top = 325 + 109 * i
            self.text(f"0{i + 1}", 44, top + 2, 12, "Bold", ORANGE)
            self.text(item["title"], 81, top, 20, "Bold")
            self.paragraph(item["body"], 81, top + 33, 467, 11.4, 16, MUTED, 64)
        self.end_page()

    def cycle(self):
        d = self.copy["cycle"]
        self.start_page(d["kicker"])
        self.title(d["headline"], 130, 37, 43)
        self.paragraph(d["intro"], 44, 239, 501, 12, 18, MUTED, 56)
        self.rect(0, 325, WIDTH, 348, INK)
        positions = [(44, 349), (330, 349), (330, 512), (44, 512)]
        for i, (item, (x, top)) in enumerate(zip(d["steps"], positions)):
            self.text(f"0{i + 1}", x, top, 24, "Black", ORANGE)
            self.text(item["title"], x, top + 39, 10, "Bold", WHITE)
            self.paragraph(item["body"], x, top + 64, 213, 11.4, 16, LIGHT, 80)
        self.arrow(264, 370, 309, 370)
        self.arrow(529, 466, 529, 504)
        self.arrow(309, 532, 265, 532)
        self.arrow(59, 504, 59, 466)
        self.text(d["foundation_title"], 44, 703, 20, "Bold")
        self.paragraph(d["foundation_body"], 44, 742, 502, 10.5, 14.5, MUTED, 44)
        self.end_page()

    def join(self):
        d = self.copy["join"]
        self.start_page(d["kicker"])
        self.title(d["headline"], 130, 37, 43)
        self.paragraph(d["intro"], 44, 237, 500, 12.3, 18, MUTED, 55)
        for i, item in enumerate(d["contributions"]):
            x = 44 + i * 175
            self.line(x, 324, x + 151, 324, ORANGE, 2)
            self.text(item["title"], x, 342, 16, "Bold")
            self.paragraph(item["body"], x, 375, 150, 10.5, 15, MUTED, 75)
        self.rect(0, 474, WIDTH, 182, INK)
        self.text(d["status_title"], 44, 491, 9, "Bold", ORANGE)
        self.text(d["status_now_label"], 44, 520, 10.8, "Bold", WHITE)
        self.paragraph(d["status_now"], 170, 520, 376, 10.7, 15.5, LIGHT, 62)
        self.text(d["status_next_label"], 44, 582, 10.8, "Bold", WHITE)
        self.paragraph(d["status_next"], 170, 582, 376, 10.7, 15.5, LIGHT, 64)
        cta_size = 21 if self.copy["language"] == "tr" else 18
        self.text(d["cta"], 44, 687, cta_size, "Bold")
        self.text(d["link_label"], 44, 727, 11.5, "Bold", "#B23822")
        self.canvas.linkURL(d["link"], (44, HEIGHT - 747, 365, HEIGHT - 723), relative=0)
        self.paragraph(d["cta_body"], 44, 755, 368, 10.4, 14, MUTED, 28)
        qr = QrCodeWidget(d["link"], barFillColor=HexColor(INK),
                          barStrokeColor=HexColor(INK), barBorder=4, barLevel="M")
        x0, y0, x1, y1 = qr.getBounds()
        side = 99
        drawing = Drawing(side, side, transform=[side / (x1 - x0), 0, 0,
                                                side / (y1 - y0), 0, 0])
        drawing.add(qr)
        renderPDF.draw(drawing, self.canvas, 455, HEIGHT - 780)
        self.canvas.linkURL(d["link"], (455, HEIGHT - 780, 554, HEIGHT - 681), relative=0)
        self.end_page()

    def build(self):
        for method in [self.cover, self.why, self.cycle, self.join]:
            method()
        self.canvas.save()
        return self.boxes


def transcript(d):
    tr = d["language"] == "tr"
    lang = d["language"]
    companion = "../en/brochure.md" if tr else "../tr/brochure.md"
    lead = "Broşür metni ve üretim kaydı" if tr else "Brochure copy and production record"
    lines = [f"# {d['title']} — {lead}", "",
             f"[{'English canonical' if tr else 'Türkçe'}]({companion}) · {d['edition']}", "",
             f"[PDF](../../assets/eylema-brochure/why-eylema.{lang}.pdf) · "
             f"[{'Düzenlenebilir metin' if tr else 'Editable copy'}](../../assets/eylema-brochure/copy.{lang}.json)", ""]
    for key in ["cover", "why", "cycle", "join"]:
        section = d[key]
        lines += ["## " + " ".join(section["headline"]), ""]
        if "subhead" in section:
            lines += ["**" + section["subhead"] + "**", ""]
        lines += [section["intro"], ""]
        for item in section.get("reasons", section.get("steps", section.get("contributions", []))):
            lines += ["### " + item["title"], "", item["body"], ""]
        if key == "cover":
            lines += [" ".join(section["statement"]), "", section["caption"], "", section["status"], ""]
        if key == "cycle":
            lines += ["### " + section["foundation_title"], "", section["foundation_body"], ""]
        if key == "join":
            lines += ["### " + section["status_title"], "", section["status_now"], "", section["status_next"], "",
                      "**" + section["cta"] + "**", "", section["cta_body"], "",
                      f"[{section['link_label']}]({section['link']})", ""]
    lines += ["## " + ("Üretim ve kontrol" if tr else "Production and verification"), ""]
    if tr:
        lines += ["Dört A4 sayfa; mevcut Eylema logosu, turuncu, mürekkep ve kâğıt renkleri. PDF, dijital paylaşım ve normal A4 baskı için hazırlanmıştır; kesim payı veya CMYK baskı profili içermez. Markdown metni erişilebilir okuma ve düzenleme karşılaştırması içindir; PDF etiketli bir erişilebilirlik belgesi değildir.", "",
                  "Metnin kaynağı ilgili JSON dosyasıdır. Üretici PDF'yi ve bu Markdown metnini aynı girdiden oluşturur. İngilizce ve Türkçe anlamları gözden geçirildikten sonra çeviri parmak izleri yenilenir. Parmak izleri anlamsal eşdeğerlik kanıtı değildir.", ""]
    else:
        lines += ["Four A4 pages using the existing Eylema logo, orange, ink and paper colors. The PDF is for digital sharing and ordinary A4 printing, with no press bleed or CMYK profile. The Markdown transcript supports accessible reading and copy comparison; the PDF is not a tagged accessibility document.", "",
                  "The corresponding JSON file is the copy source. The builder generates the PDF and this Markdown transcript from the same input. Refresh translation fingerprints after reviewing English and Turkish meanings. Fingerprints do not prove semantic equivalence.", ""]
    lines += ["```sh", "python tools/render-brochure.py --font-dir <licensed-arial-directory> --verify",
              ".venv/bin/python tools/check_repository.py --write-translation-hashes", "```", ""]
    lines += [("Üretim araçları uygulama çalışma zamanına veya CI'ya eklenmez. " if tr else "Authoring tools are not added to the application runtime or CI. ") +
              ("Sürümler, kaynak ve çıktı özetleri " if tr else "Versions, source and output hashes are recorded in ") +
              "[tooling.json](../../assets/eylema-brochure/tooling.json)" + (" dosyasında kayıtlıdır." if tr else "."), "",
              ("Araç mevcut kurulumları kullanır; ağdan indirme veya kurulum yapmaz. Yazı tipi dosyaları dağıtılmaz; PDF'ye yalnız kullanılan karakterlerin alt kümeleri gömülür. Lisanslı Arial Regular, Bold ve Black gerekir. Görsel kontrollerin çıktıları `.cache/brochure/` altındadır." if tr else
               "The tool uses existing installations, with no network downloads or installs. Font files are not distributed; only used glyph subsets are embedded in the PDF. Licensed Arial Regular, Bold and Black are required. Visual-check outputs are kept under `.cache/brochure/`."), "",
              "## " + ("İddiaların kaynakları" if tr else "Claim sources"), "",
              ("Mevcut durum için çekirdek deposunun aşağıdaki revizyonu esas alınmıştır. Daha eski tasarım belgelerindeki durum cümleleri bu ilerlemeyi geçersiz kılmaz. Geliştirme, yerel işletim, insan denetimi ve öğrenme döngüsü; tamamlanmış ürün özellikleri olarak sunulmaz." if tr else
               "Current status follows the core revision below. Older status sentences in design documents do not override that progress. Development, local operation, human oversight and the learning cycle are not presented as completed product capabilities."), ""]
    lines += [f"- [{name}]({url})" for name, url in SOURCES.items()]
    lines += ["", ("Bu broşür yeni çalışma zamanı, GPU, model, eğitim veya performans kanıtı üretmez. İnceleme bu Codex oturumunda öz incelemedir; bağımsız inceleme beklemektedir." if tr else
                    "This brochure creates no new runtime, GPU, model, training or performance evidence. Review in this Codex context is self-review; independent review remains pending."), ""]
    return "\n".join(lines)


def normalize(text):
    return re.sub(r"\s+", " ", text).strip()


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for key, item in value.items():
            if key not in {"language", "title", "edition", "link"}:
                yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def verify(d, path, boxes):
    import pypdfium2
    from pypdf import PdfReader
    from PIL import Image
    reader = PdfReader(path)
    assert len(reader.pages) == 4, "Expected exactly four pages"
    extracted = normalize(" ".join(page.extract_text() for page in reader.pages))
    missing = [s for s in strings(d) if normalize(s) not in extracted]
    assert not missing, f"Missing or broken PDF text: {missing}"
    for page in reader.pages:
        assert abs(float(page.mediabox.width) - WIDTH) < 0.1
        assert abs(float(page.mediabox.height) - HEIGHT) < 0.1
        for font in page["/Resources"]["/Font"].values():
            font = font.get_object()
            if font["/Subtype"] == "/TrueType":
                assert "/FontFile2" in font["/FontDescriptor"], "TrueType font is not embedded"
    assert reader.metadata.title == d["title"]
    links = [a.get_object().get("/A", {}).get("/URI") for a in reader.pages[-1]["/Annots"]]
    assert links.count(d["join"]["link"]) == 2
    cache = ROOT / ".cache/brochure" / d["language"]
    cache.mkdir(parents=True, exist_ok=True)
    doc = pypdfium2.PdfDocument(path)
    thumbs = []
    for index, page in enumerate(doc):
        bitmap = page.render(scale=2)
        im = bitmap.to_pil()
        im.save(cache / f"page-{index + 1}.png")
        thumb = im.copy()
        thumb.thumbnail((360, 510))
        thumbs.append(thumb)
        bitmap.close()
        page.close()
    doc.close()
    contact = Image.new("RGB", (360 * 4 + 5 * 14, 540), "#D9DDD8")
    for i, im in enumerate(thumbs):
        contact.paste(im, (14 + i * 374, 14))
    contact.save(cache / "overview.png")
    (cache / "text.txt").write_text(extracted + "\n")
    (cache / "layout.json").write_text(json.dumps(boxes, ensure_ascii=False, indent=2) + "\n")
    print(f"PASS {d['language']}: four A4 pages; complete text; embedded fonts; two links; rendered all pages")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--font-dir", type=Path, default=Path("/System/Library/Fonts/Supplemental"))
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    fonts = {"Regular": "Arial.ttf", "Bold": "Arial Bold.ttf", "Black": "Arial Black.ttf"}
    for name, filename in fonts.items():
        path = args.font_dir / filename
        if not path.is_file():
            parser.error(f"Missing font {filename}; supply a licensed Arial directory with --font-dir")
        pdfmetrics.registerFont(TTFont(name, str(path)))
    copies = [json.loads((OUTPUT / f"copy.{lang}.json").read_text()) for lang in ["en", "tr"]]
    records = []
    for d in copies:
        lang = d["language"]
        path = OUTPUT / f"why-eylema.{lang}.pdf"
        boxes = Brochure(d, path).build()
        (ROOT / f"docs/{lang}/brochure.md").write_text(transcript(d))
        if args.verify:
            verify(d, path, boxes)
        records.append({"language": lang, "copy_sha256": digest(OUTPUT / f"copy.{lang}.json"),
                        "pdf_sha256": digest(path), "transcript_sha256": digest(ROOT / f"docs/{lang}/brochure.md")})
    inventory = {
        "purpose": "Editorial authoring and optional artifact QA only; no runtime or CI dependency change",
        "runtime": "Bundled Codex Python " + platform.python_version() + "; repository gates use their separate Python 3.14 environment",
        "packages": {name: importlib.metadata.version(name) for name in
                     ["reportlab", "Pillow", "pypdf", "pypdfium2"]},
        "font_policy": "Read licensed installed Arial fonts; distribute PDF glyph subsets only",
        "fonts": {filename: digest(args.font_dir / filename) for filename in fonts.values()},
        "sources": SOURCES,
        "outputs": records,
        "pdf_intent": "Four A4 pages, digital/ordinary print; no bleed or CMYK; not tagged for PDF accessibility",
    }
    (OUTPUT / "tooling.json").write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
