from .services import OrderService
from .models import Order
from .utils import format_price
from .config import DEFAULT_CURRENCY as CURRENCY
import shop.config as config
from . import config as sibling_config
from .models import *  # star import — should produce diagnostic
import importlib  # dynamic import — should not resolve
