import { router } from "@kit.ArkUI";
import { Logger } from "../utils/Logger";
import { Second } from "./Second";

@Entry
@ComponentV2
struct Index {
  @Local message: string = "Hello World";
  @Local count: number = 0;

  build() {
    Column() {
      Text(this.message)
        .fontSize(28)
        .fontWeight(FontWeight.Bold)
      Button("Next")
        .onClick(() => {
          Logger.info("Navigating to Second");
          router.pushUrl({ url: "pages/Second" });
        })
      Second({ initialCount: this.count })
    }
    .width("100%")
    .height("100%")
  }
}
