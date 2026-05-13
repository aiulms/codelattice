import { Logger, formatLog } from "../utils/Logger";

@Component
struct Second {
  @State count: number = 0;
  @Prop initialCount: number = 0;

  aboutToAppear() {
    Logger.info(formatLog("Second", `Initial: ${this.initialCount}`));
  }

  build() {
    Row() {
      Column() {
        Text(`Count: ${this.count}`)
          .fontSize(20)
        Button("Increment")
          .onClick(() => {
            this.count++;
            Logger.info(`Count is now ${this.count}`);
          })
      }
    }
  }
}
