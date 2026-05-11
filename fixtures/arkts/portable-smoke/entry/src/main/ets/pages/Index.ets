import { router } from "@kit.ArkUI";

@Entry
@ComponentV2
struct Index {
  @Local message: string = "Hello World";

  build() {
    Column() {
      Text(this.message)
        .fontSize(28)
        .fontWeight(FontWeight.Bold)
      Button("Next")
        .onClick(() => {
          router.pushUrl({ url: "pages/Second" });
        })
    }
    .width("100%")
    .height("100%")
  }
}
